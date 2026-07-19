from openai import OpenAI

client = OpenAI(
    api_key="sk-43cb6231d1672ba25e483642cdff64f1c5c52ec81624cbd21d298132de6c6473",
    base_url="https://sub.fuck-gpt.cyou/v1"
)

# 获取模型列表
models = client.models.list()
for model in models.data:
    print(model.id)
