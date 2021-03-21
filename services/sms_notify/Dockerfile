# BUILDER
FROM python:3 as builder

WORKDIR /

# Activate virtualenv
RUN python -m venv /opt/venv

# Make sure we use the virtualenv
ENV PATH="/opt/venv/bin:$PATH"

# Copy requirements and build with pip
COPY requirements.txt ./
RUN pip install -r requirements.txt



# RUNTIME
FROM python:3-slim as runtime

WORKDIR /

# Copy compiled venv from builder
COPY --from=builder /opt/venv /opt/venv

# Make sure we use the virtualenv
ENV PATH="/opt/venv/bin:$PATH"

# Copy script over and run
COPY sms_notify.py .
COPY fileenv.sh .
CMD [ "bash", "/fileenv.sh" ]
