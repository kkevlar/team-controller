from flask import Flask, render_template, request, redirect
import requests
import time

app = Flask(__name__)

@app.route('/')
def index():
    return render_template('index.html')

@app.route('/team1', methods=['POST'])
def team1():
    data = {'teams': 1}
    requests.post('http://localhost:5001/team', json=data)
    return render_template('success.html', redirect_url='/')
@app.route('/team2', methods=['POST'])
def team2():
    data = {'teams': 2}
    requests.post('http://localhost:5001/team', json=data)
    return render_template('success.html', redirect_url='/')
@app.route('/team3', methods=['POST'])
def team3():
    data = {'teams': 3}
    requests.post('http://localhost:5001/team', json=data)
    return render_template('success.html', redirect_url='/')
@app.route('/team4', methods=['POST'])
def team4():
    data = {'teams': 4}
    requests.post('http://localhost:5001/team', json=data)
    return render_template('success.html', redirect_url='/')

@app.route('/setup', methods=['POST'])
def reset():
    requests.post('http://localhost:5001/setup')
    return render_template('success.html', redirect_url='/')

@app.route('/start', methods=['POST'])
def start():
    requests.post('http://localhost:5001/start')
    return render_template('success.html', redirect_url='/')

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000, debug=True)

