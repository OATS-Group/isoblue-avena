#!/usr/bin/python3
import sys
import asyncio
from time import sleep
from nats.aio.client import Client as NATS
from nats.aio.errors import ErrConnectionClosed, ErrTimeout, ErrNoServers

async def run(loop):

    print('Setting up NATS')
    sys.stdout.flush()

    # Connect to NATS server
    nc = NATS()
    await nc.connect('nats://nats:4222')
    count = 0
    sequence = [0, 0, 0, 1, 1, 1, 0, 1]
    while True:
      subject = 'sms'
      content = str(sequence[count % len(sequence)])
      print('Publishing on subject', subject, 'with content', content);
      sys.stdout.flush()
      await nc.publish(subject, bytes(content, 'utf-8'))
      await nc.flush(1)
      count = count + 1

      sleep_time = 5
      print("Sleeping", sleep_time)
      sleep(sleep_time)

    await nc.close()

if __name__ == '__main__':
    loop = asyncio.get_event_loop()
    loop.run_until_complete(run(loop))
    loop.close()

    sys.exit(0)
