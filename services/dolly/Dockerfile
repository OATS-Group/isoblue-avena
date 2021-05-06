FROM rclone/rclone:latest

# Add dolly script
ADD go-dolly /

# Make script executable
RUN chmod +x /go-dolly

# Don't do anything. Let ofelia control when we run
ENTRYPOINT sleep infinity
