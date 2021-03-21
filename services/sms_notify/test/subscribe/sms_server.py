# Built-in library
import json
import sys

# Web server stuff taken from here: 
# https://medium.com/analytics-vidhya/asynchronous-web-server-in-python-eac521fba518
# Third-party library
from aiohttp import web
from nats.aio.client import Client as NATS
from nats.aio.errors import ErrConnectionClosed, ErrTimeout, ErrNoServers


host = '0.0.0.0'
port = 1117
class WebServer:
    def __init__(self, host_arg, port_arg):
        self.app = web.Application()
        self.host = host_arg
        self.port = port_arg

    async def initializer(self) -> web.Application:
        # Setup routes and handlers
        self.app.router.add_post('/sms', self.postHandler)

        print("Setting up NATS")
        sys.stdout.flush()
        self.nc = NATS()

        print("Connecting to nats server")
        await self.nc.connect("nats://nats:4222")
       
        return self.app
    
    async def postHandler(self, request: web.Request) -> web.Response:
        print("Received post request with the following body:", request)
        data = await request.post()
        print("Text message sent from",data["From"], "with body `",data["Body"],"`")

        subject = "isoblue.notifications.sms.new_subscriber"
        content = data["From"]

        print('Publishing on subject', subject, 'with content', content);
        sys.stdout.flush()
        await self.nc.publish(subject, bytes(content, 'utf-8'))
        await self.nc.flush(1)

    
        sys.stdout.flush()
        return web.HTTPOk()
    
    def run(self):
        web.run_app(self.initializer(), host=self.host, port=self.port)

if __name__ == '__main__':
    print("Creating a webserver on host", host, "port", port)
    webserver = WebServer(host, port)
    webserver.run()
