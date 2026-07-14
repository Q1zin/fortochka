# Деплой

Автодеплой срабатывает на каждый push в master: CI собирает образ
`ghcr.io/q1zin/fortochka-server`, заливает compose-файлы на сервер и делает
`docker compose pull && up -d`. Ниже — разовая настройка.

## 1. GitHub Secrets (Settings → Secrets and variables → Actions)

| Secret | Что это |
|--------|---------|
| `SSH_HOST` | IP или хост сервера |
| `SSH_USER` | пользователь для SSH |
| `SSH_PRIVATE_KEY` | приватный deploy-ключ (создание — ниже) |

Создать отдельный deploy-ключ (не личный!):

```bash
ssh-keygen -t ed25519 -f ~/.ssh/fortochka_deploy -C "fortochka-deploy" -N ""
ssh-copy-id -i ~/.ssh/fortochka_deploy.pub <user>@<host>
cat ~/.ssh/fortochka_deploy   # содержимое → секрет SSH_PRIVATE_KEY
```

## 2. Разовая настройка сервера

```bash
# Docker, если ещё нет
curl -fsSL https://get.docker.com | sh

sudo mkdir -p /opt/fortochka && sudo chown $USER /opt/fortochka
echo "FORTOCHKA_DOMAIN=fortochka.fun" > /opt/fortochka/.env
```

Если репозиторий (а значит и GHCR-пакет) приватный — серверу нужен доступ
на чтение пакетов: создай PAT c правом `read:packages` и выполни на сервере
`docker login ghcr.io -u Q1zin`.

## 3. DNS и порты

- A-запись `fortochka.fun` → IP сервера;
- открыты порты 80 и 443 (Caddy сам получит сертификат Let's Encrypt).

## 4. Проверка

Сквозной smoke-тест (register → upload → pair → wallpaper):

```bash
just smoke                              # против https://fortochka.fun
just smoke http://127.0.0.1:8080        # против локального сервера
```

Либо руками:

```bash
curl https://fortochka.fun/healthz      # → ok

curl -s -X POST https://fortochka.fun/api/v1/cameras/register \
  -H 'content-type: application/json' -d '{"name":"Тест"}'
# → camera_id, upload_token, pairing_code

curl -s -X POST https://fortochka.fun/api/v1/cameras/<camera_id>/frame \
  -H "authorization: Bearer <upload_token>" \
  -H 'content-type: image/jpeg' --data-binary @кадр.jpg

curl -s -X POST https://fortochka.fun/api/v1/pair \
  -H 'content-type: application/json' -d '{"pairing_code":"<код>"}'
# → view_token

curl -s "https://fortochka.fun/cam/<view_token>/wallpaper.jpg?w=1080&h=2400" -o обои.jpg
```

Откат на предыдущую версию: на сервере
`docker compose up -d server --force-recreate` с образом по sha
(`ghcr.io/q1zin/fortochka-server:<git-sha>` вместо `latest` в compose).
