FROM alpine:latest
COPY demo.sh /bin/demo.sh
RUN chmod +x /bin/demo.sh
ENTRYPOINT ["/bin/demo.sh"]
