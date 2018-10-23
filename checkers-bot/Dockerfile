FROM node:10-alpine

WORKDIR /usr/src

ADD package.json package-lock.json ./
RUN npm i
ADD * ./
RUN npm i -g eslint && eslint index.js && npm un -g eslint

CMD ["npm", "start"]
