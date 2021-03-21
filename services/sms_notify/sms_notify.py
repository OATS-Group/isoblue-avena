#!/usr/bin/python3
import os
import sys
import re
from time import sleep
import asyncio
from nats.aio.client import Client as NATS
from nats.aio.errors import ErrConnectionClosed, ErrTimeout, ErrNoServers
from twilio.rest import Client

async def run(loop):
    print("Setting up NATS")
    sys.stdout.flush()
    nc = NATS()

    print("Connecting to nats server")
    await nc.connect("nats://nats:4222")

    account_number = os.environ['TWILIO_ACCT_NUMBER']
    account_sid = os.environ['TWILIO_ACCOUNT_SID']
    auth_token = os.environ['TWILIO_AUTH_TOKEN']
    client = Client(account_sid, auth_token)
    print("Using phone number", account_number, "to send texts")

    notification_threshold = 1

    async_shared = type('', (), {})()
    async_shared.numbers = [ ]
    async_shared.previous = 0
    async def notify_sms(msg):
        subject = msg.subject
        reply = msg.reply
        data = msg.data.decode()
        print("Received a message on '{subject} {reply}': {data}".format(
            subject=subject, reply=reply, data=data))

        if len(async_shared.numbers) == 0:
            print('No numbers to notify')
            return

        if int(data) >= notification_threshold:
            if async_shared.previous < notification_threshold:
                for dest_num in async_shared.numbers:
                    print("Sending notification to", dest_num)
                    message = client.messages \
                                    .create(
                                         body="The number is " + data,
                                         from_=account_number,
                                         to=dest_num
                                    )
        
                    print(message.sid)
            else:
                print("Already sent a notification")
        else:
            print("Data did not suprass threshold")
        async_shared.previous = int(data)
        sys.stdout.flush()


    async def new_subscriber(msg):
        subject = msg.subject
        reply = msg.reply
        data = msg.data.decode()
        print("Received a message on '{subject} {reply}': {data}".format(
            subject=subject, reply=reply, data=data))
   
        # Do basic matching of the phone number
        if not re.match('^\+[1-9]\d{1,14}$', data):
            print("Received number does not appear to be a valid phone number")
            return

        if data not in async_shared.numbers:
            print("Adding number to db and sending confirmation response")
            async_shared.numbers.append(data)
            message = client.messages \
                            .create(
                                 body="Thank you for participating in the OATSCON 2021 ISOBlue demo. Your number will be deleted at the end of the demo.",
                                 from_=account_number,
                                 to=data
                 )

            print(message.sid)
        else:
            print("Number already in database, skipping")
        print("New set of numbers of send notifications to:",async_shared.numbers)

    notify_subject = 'sms' 
    print("Subscribing to subject", notify_subject)
    await nc.subscribe(notify_subject, cb=notify_sms)

    subscribe_subject = "isoblue.notifications.sms.new_subscriber"
    print("Subscribing to subject", subscribe_subject)
    await nc.subscribe(subscribe_subject, cb=new_subscriber)

    while True:
      await asyncio.sleep(1)

    # Gracefully close the connection.
    await nc.drain()

if __name__ == '__main__':
    loop = asyncio.get_event_loop()
    loop.run_until_complete(run(loop))
    loop.close()
