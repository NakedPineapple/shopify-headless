FROM flyio/log-shipper:latest

# Copy custom Vector config that parses JSON from message field
# Path is relative to the build context (fly/ directory)
COPY log-shipper/vector.toml /etc/vector/vector.toml
