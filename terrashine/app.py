from flask import Flask

app = Flask(__name__)

@app.route("/v1/providers/<hostname>/<namespace>/<type>/index.json")
def provider_index():
    return ""

@app.route("/v1/providers/<namespace>/<type>/<version>.json")
def provider_version():
    return ""