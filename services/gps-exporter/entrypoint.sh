#!/bin/bash

# Start the run once job.
timestamp=`date +%Y/%m/%d-%H:%M:%S`
echo "Periodic Docker container has been started at $timestamp"

# This and the SHELL/BASH_ENV allow enviromental variables to work
declare -p | grep -Ev 'BASHOPTS|BASH_VERSINFO|EUID|PPID|SHELLOPTS|UID' > /app/container.env

# Setup a cron schedule
# redirecting to proc/1/fd/1 prints to the docker log
echo "SHELL=/bin/bash
BASH_ENV=/app/container.env
*/1 * * * * /app/run-gps-exporter.sh > /proc/1/fd/1 2>&1
# This extra line makes it a valid cron" > /app/scheduler.txt

crontab /app/scheduler.txt
cron -f
crontab -l
