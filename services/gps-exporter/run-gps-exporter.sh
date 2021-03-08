#!/bin/bash

echo "\$db_user = $db_user"
echo "\$db_host = $db_host"
echo "\$db_database = $db_database"
echo "\$from = $from"
echo "\$segment_length = $segment_length"

/app/gps-exporter postgress://$db_user:$db_password@$db_host:$db_port/$db_database /data -f "$from" -l $segment_length --append
