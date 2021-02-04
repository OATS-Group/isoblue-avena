#!/usr/bin/env python3

import socket
import postgres
import csv
import os
import time
import sys
import collections
import multiprocessing as mp
from datetime import datetime
from psycopg2 import OperationalError
from psycopg2.extras import execute_values


def write_to_csv(wr_buff, can_bus):

    # Write to CSV from write buffer. First item is timestamp, second is
    # can_id and third is can_data

    with open(f'/data/log/{can_bus}.csv', mode='a') as logfd:

        logcsv = csv.writer(logfd, delimiter=',', quotechar='"',
                            quoting=csv.QUOTE_MINIMAL)

        logcsv.writerows(wr_buff)

    print(f'{can_bus}: Wrote successfully to CSV')


def write_to_db(db, wr_buff, can_bus):
    
    try:

        with db.get_cursor() as cursor:
            
            execute_values(cursor,
                           "INSERT INTO can(time, can_interface, can_id, can_data) \
                           VALUES %s;", wr_buff)

    except(Exception, SyntaxError):

        print(f'{can_bus}: An error occured while inserting data to database')

    else:

        print(f'{can_bus}: Wrote successfully to DB')


def db_init():

    connection_url = ('postgresql://' + os.environ['db_user'] + ':' +
                      os.environ['db_password'] + '@postgres:' +
                      os.environ['db_port'] + '/' + os.environ['db_database'])

    print('Initializing Postgres Object...')
    db = postgres.Postgres(url=connection_url)
    print('Ensuring timescaledb ext. is enabled')
    db.run("CREATE EXTENSION IF NOT EXISTS timescaledb;")
    print("Ensuring tables are setup properly")
    db.run("""
           CREATE TABLE IF NOT EXISTS can (
               time timestamptz NOT NULL,
               can_interface text NOT NULL,
               can_id text NOT NULL,
               can_data text NOT NULL);""")

    print("Ensuring can data table is a timescaledb hypertable")
    db.run("""
           SELECT create_hypertable('can', 'time', if_not_exists => TRUE,  
           migrate_data => TRUE);""")

    print("Finished setting up tables")

    return db

# This detection function was taken from can_watchdog. Author: Aaron Neustedter

def detect_can_interfaces():

    can_interfaces = []

    print('Gathering all can interfaces')

    sysclass = '/mnt/host/sys/class/net/'

    # Iterate through all links listed in /sys/class/net
    for network in os.listdir(sysclass):

        # This file defines the type of the network
        path = sysclass + network + '/type'
        print(f'Checking network  {network},  type at path {path}')

        # Sometimes things are not setup like we expect. Live and let live
        if not os.path.isfile(path):
            print(f'{network} does not have a type file. Skipping')
            continue

        # Open the file and read it
        with open(path) as typefile:
            networktype = typefile.read().strip()

        # 280 is the type for CAN. 'Documentation' here:
        # https://elixir.bootlin.com/linux/latest/source/include/uapi/linux/if_arp.h#L56
        if networktype.isdigit() and int(networktype) == 280:
            print('\t', network, ' appears to be a CAN link')
            can_interfaces.append(network)

    if len(can_interfaces) <= 0:
        print('FATAL: No CAN interfaces found')
        sys.exit(-1)

    print(len(can_interfaces), ' found: ', can_interfaces)

    return can_interfaces


def log_can(can_interface):

    print(f'Logging {can_interface}')
     
    frame = ''
    rx_buff = []
    wr_buff = []
    socket_connected = False
    # Python deque to store the last 3 received elements from the socket
    buff = collections.deque(maxlen=3)
    
    # Initialize socket
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    # Connect to socketcand

    while(socket_connected is False):

        try:

            s.connect((host_ip, int(host_port)))

        except(ConnectionRefusedError):

            print('Could not connect to socketcand. Connection Refused. \
                    Retrying...')
            time.sleep(10)
            socket_connected = False

        else:

            print('Successfully connected to socketcand at',
                  f'{host_ip}: {host_port}')
            socket_connected = True
    sys.stdout.flush()

    # Receive socketcand's response. After each command, socketcand replies
    # < ok > if the command was successful. Each reply must be received before
    # sending new commands, else, socketcand won't receive new commands. For
    # more details about socketcand's protocol, please refer to: 
    # https://github.com/linux-can/socketcand/blob/master/doc/protocol.md

    s.recv(32)

    # Connect to exposed CAN interface and receive socketcand's respone.

    s.sendall(b'< open ' + can_interface.encode('utf-8') + b' >')

    s.recv(32)

    # Set socket to 'rawmode' to receive every frame on the bus.
    s.sendall(b'< rawmode >')

    s.recv(32)

    # Receive data in a 54-byte long socket buffer. Data may come split and 
    # incomplete after each iteration, so data received from the socket 
    # buffer is copied to a circular buffer (buff). This circular buffer
    # stores up to three messages to ensure a complete frame can be obtained.
    # After filling this buffer, information is converted to a list and stored
    # in a "frame buffer". This buffer contains data received from the last 
    # three iterations. After filling the frame_buffer, a complete frame is 
    # obtained by concatenating the second and third elements of the frame buffer.
    # Then the resulting bytes element is encoded to a UTF-8 string and its data
    # obtained using string manipulation. New frames always start with "<", so
    # the string is split after each occurence of this character. Afterwards,
    # the second element of the resulting list will contain the full data.
    # Finally, some characters are stripped to clean up the received frame 
    # and then split the resulting string to get the timestamp, CAN ID 
    # and CAN frame.

    while(True):
        sys.stdout.flush()
 
        
        # Buffer to store raw bytes received from the socket.
        socket_buff = s.recv(54)
        buff.append(socket_buff)
        
        # List representation of buff
        frame_buff = list(buff)
        
        if(len(frame_buff) > 2):

            # Decoded and assembled version of frame_buff in string format 
            frame = frame_buff[1] + frame_buff[2]
            frame = frame.decode("utf-8").split("<")
            frame = frame[1].strip('>').split(' ')

        try:

            (timestamp, can_bus, can_id, can_data) = (frame[3], can_interface,
                                                      frame[2], frame[4])

        except(IndexError):

            print(f'Error logging CAN frame at {can_interface}. Skipping...')

        else:

            timestamp = datetime.fromtimestamp(float(timestamp)).isoformat()
            rx_buff.append((timestamp, can_bus, can_id, can_data))

        # When the receive buffer reaches 1000 entries, copy data from receive
        # buffer to write buffer, then write to database from write buffer and
        # clear receive buffer to continue receiving data
        
        if(len(rx_buff) >= 1000):

            wr_buff.clear()
            wr_buff = rx_buff.copy()
            rx_buff.clear()

            if(logtodb):

                p_db = mp.Process(target=write_to_db, args=(db, wr_buff,
                                                            can_bus,))
                p_db.start()

            if(logtocsv):

                p_csv = mp.Process(target=write_to_csv, args=(wr_buff,
                                                              can_bus,))
                p_csv.start()

# Get host info using environment variables

host_ip = os.environ['socketcand_ip']
host_port = os.environ['socketcand_port']
host_interfaces = os.environ['can_interface']
logging = os.environ['log']

# Split host_interfaces string into a list of strings.

host_interfaces = host_interfaces.split(',')

# Initialize variables

can_interfaces = []
logtodb = False
logtocsv = False
socket_connected = False
db_started = False

# Check log selection from env. variables

if (logging.find('db') != -1):

    logtodb = True

if (logging.find('csv') != -1):

    logtocsv = True

# Detect available CAN interfaces

avail_interfaces = detect_can_interfaces()

print("Detected interfaces: " + str(avail_interfaces))

# Check selected CAN interfaces in env variable are available.

for i in host_interfaces:

    if i in avail_interfaces:

        can_interfaces.append(i)

    else:

        print(f'Interface {i} is not valid or is not currently available')

# Initialize postgres database if database logging is enabled. The database
# sometimes is not ready to accept connections. In that case, report the issue
# and wait 10 seconds to try again. Keep trying until a successful connection
# can be made.

if (logtodb):

    while(db_started is False):

        try:

            db = db_init()

        except(OperationalError):

            print('Error: Database system has not been started up', 
                  'or is starting up. Waiting...')
            time.sleep(10)
            db_started = False

        else:

            db_started = True

for can_bus in can_interfaces:
    print('Creating process for', can_bus)
    mp.Process(target=log_can, args=(can_bus,)).start()
