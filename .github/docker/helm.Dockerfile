FROM hubbleprotocol/helm:0.0.1

ARG BUILD_VERSION
ENV CHART=scope-bot

COPY /off_chain/docker-bot/helm /build/helm
COPY /.github/scripts /build/scripts
WORKDIR /build/helm

RUN ../scripts/update-local-chart.sh "hubbleprotocol/$CHART" "$BUILD_VERSION"

RUN --mount=type=secret,id=aws_access_key_id \
	--mount=type=secret,id=aws_secret_access_key \
	export AWS_ACCESS_KEY_ID=$(cat /run/secrets/aws_access_key_id)  \
	&& export AWS_SECRET_ACCESS_KEY=$(cat /run/secrets/aws_secret_access_key) \
	&& export AWS_REGION=eu-west-1 \
	&& helm repo add hubbleprotocol s3://helmrepository.build.hubbleprotocol.io \
	&& helm dependency update \
	&& helm package ./ \
	&& helm s3 push ./"$CHART"-"$BUILD_VERSION".tgz hubbleprotocol
