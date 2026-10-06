FROM python:3.12-slim
WORKDIR /app
COPY requirements.txt requirements.lock ./
RUN pip install --no-cache-dir -r requirements.txt && useradd --uid 10001 --create-home proxy
COPY cloud/ cloud/
RUN mkdir /data && chown proxy:proxy /data
USER proxy
ENV PROXY_DATABASE=/data/proxy.sqlite3
EXPOSE 8000
CMD ["uvicorn", "cloud.app:create_app", "--factory", "--host", "0.0.0.0", "--port", "8000", "--no-access-log"]
