# Reverse proxy

The terraform [provider network mirror protocol](https://developer.hashicorp.com/terraform/internals/provider-network-mirror-protocol) requires that the API request be performed over encrypted HTTPS.
Terrashine itself does not currently perform TLS termination, a reverse proxy must always be deployed to perform this function for a working setup.

## Securing the admin API

Terrashine provides an API endpoint which should be protected by the reverse proxy.
Endpoints hosted under the `/api/` should be considered privileged and not exposed externally without an authentication layer.
Currently, authentication should be implemented by the reverse proxy and is not natively supported by terrashine.

## External Caching

Caching is optional however, terrashine sets `Cache-Control` headers where possible to allow caching by external reverse proxies.
If caching is required, this should be achieved by configuring the reverse proxy to cache responses as appropriate.
Cache headers are sometimes not set in cases where caching may incorrect behavior by the terraform client.
For example: headers are not set in scenarios where caching could result in subsequent requests from the same client seeing inconsistent views of the available packages, resulting in an error when downloading packages.

## Example NGINX configuration

Here is an example NGINX configuration that provides TLS termination and caching enabled
for a locally deployed terrashine instance.

``` nginx
user  nginx;
worker_processes  auto;

error_log  /dev/stdout notice;
pid        /var/run/nginx.pid;

events {
    worker_connections  1024;
}

http {
    default_type  application/octet-stream;

    log_format  main  '$remote_addr - $remote_user [$time_local] "$request" '
                      '$status $body_bytes_sent "$http_referer" '
                      '"$http_user_agent" "$http_x_forwarded_for"';
    access_log  /dev/stdout  main;
    sendfile        on;
    keepalive_timeout  65;
    proxy_cache_path /tmp keys_zone=mycache:10m;


    server {
        listen              443 ssl;
        server_name         localhost;
        proxy_cache         mycache;
        ssl_certificate     localhost.pem;
        ssl_certificate_key localhost.pem;
        ssl_protocols       TLSv1 TLSv1.1 TLSv1.2;
        ssl_ciphers         HIGH:!aNULL:!MD5;
        location / {
            # terrashine
            proxy_pass http://localhost:9543;
        }
        # Deny traffic to the API endpoint
        # This could be protected by basic auth as well
        location /api {
            deny all;
            return 404;
        }
    }
}
```
