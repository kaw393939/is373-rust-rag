FROM python:3.13-slim-bookworm
WORKDIR /service
COPY deploy/embeddings/requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt && groupadd -g 10001 ordo && useradd -u 10001 -g ordo -m ordo && mkdir /models && chown ordo:ordo /models
COPY deploy/embeddings/server.py deploy/embeddings/model-lock.json ./
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 TOKENIZERS_PARALLELISM=false OMP_NUM_THREADS=1 HF_HUB_OFFLINE=1
USER 10001:10001
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 CMD python -c "import urllib.request;urllib.request.urlopen('http://127.0.0.1:8080/health',timeout=3)"
ENTRYPOINT ["python","/service/server.py"]
