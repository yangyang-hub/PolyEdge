# 前端静态文件由本机 yarn build 预构建到 out/。
# Docker 镜像使用 Nginx 托管静态文件，并将同源 /api 请求代理到 polyedge-server。
FROM nginx:1.27-alpine

COPY nginx.conf.template /etc/nginx/conf.d/polyedge.conf
COPY out/ /usr/share/nginx/html
RUN rm -f /etc/nginx/conf.d/default.conf

EXPOSE 80
