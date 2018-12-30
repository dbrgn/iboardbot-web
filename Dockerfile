FROM rust:1-stretch

COPY . /opt/iboardbot/
RUN cd /opt/iboardbot && cargo build --release

RUN mkdir /iboardbot/ \
 && adduser --disabled-password --gecos "" iboardbot \
 && chown iboardbot:iboardbot /iboardbot/ \
 && chmod 0700 /iboardbot/

RUN cp /opt/iboardbot/target/release/iboardbot-web /usr/local/bin/iboardbot-web \
 && cp -r /opt/iboardbot/static /iboardbot/static
RUN sh -c "echo '{\"static_dir\": \"/iboardbot/static\", \"listen\": \"0.0.0.0:8080\"}' > /iboardbot/config.json"

WORKDIR /iboardbot

USER iboardbot

CMD [ "iboardbot-web", "-c", "/iboardbot/config.json" ]
