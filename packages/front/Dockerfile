# 前端静态文件由本机 yarn build 预构建到 out/，
# Docker 镜像只负责 nginx 托管静态文件，API 由浏览器直连后端。
FROM nginx:1.27-alpine

COPY nginx.conf.template /etc/nginx/conf.d/polyedge.conf
COPY out/ /usr/share/nginx/html
RUN rm -f /etc/nginx/conf.d/default.conf

EXPOSE 80
