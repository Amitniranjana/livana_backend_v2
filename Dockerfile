# Use the latest Rust image
FROM rust:latest

# Install system dependencies
RUN apt-get update && \
    apt-get install -y libpq-dev && \
    rm -rf /var/lib/apt/lists/*

# Set the working directory inside the container
WORKDIR /app/

# Copy the current directory contents into the container at /app/
COPY . .



# Build your project, this compiles the actual binary
RUN cargo build --release

# After building, the binary will be located at /app/target/release/my_app
# Set the default command to run your compiled binary
CMD ["./target/release/livana-backend-v2"]