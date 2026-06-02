# 前端静态文件由本机 yarn build 预构建到 out/，
# Docker 镜像只负责 nginx 托管，不再容器内编译。
FROM nginx:1.27-alpine

ENV POLYEDGE_API_UPSTREAM=http://polyedge-api:38001
ENV POLYEDGE_CONSOLE_STEP_UP_CODE=__unset__

COPY nginx.conf.template /etc/nginx/polyedge.conf.template
COPY out/ /usr/share/nginx/html
RUN rm -f /etc/nginx/conf.d/default.conf

EXPOSE 80
CMD ["sh", "-c", "test -n \"$POLYEDGE_CONSOLE_STEP_UP_CODE\" && test \"$POLYEDGE_CONSOLE_STEP_UP_CODE\" != \"__unset__\" || { echo 'POLYEDGE_CONSOLE_STEP_UP_CODE is required for the static frontend proxy.' >&2; exit 1; }; envsubst '$${POLYEDGE_API_UPSTREAM} $${POLYEDGE_CONSOLE_STEP_UP_CODE}' < /etc/nginx/polyedge.conf.template > /etc/nginx/conf.d/polyedge.conf && nginx -g 'daemon off;'"]
