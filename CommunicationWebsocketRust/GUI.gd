extends Control

# the session id for HTTP requests
var session_id = ""
# the username we used to connect
var username = ""
# object for handling websocket events (connection, data reception...)
var ws_client = WebSocketClient.new()

# For HTTP requests queing (see do_request and request_one)
var requests = []

# Called when the node enters the scene tree for the first time.
func _ready():
	
	# connect websocket signals to this GUI's slots
	ws_client.connect("connection_established", self, "_ws_connected")
	ws_client.connect("connection_error", self, "_ws_closed")
	ws_client.connect("connection_closed", self, "_ws_closed")
	ws_client.connect("data_received", self, "_ws_data")
	
	# Set the HTTP response handler to _login_request_handler
	# as we first need to connect to the chat before
	# handling message polls
	$HTTPRequest.connect("request_completed", self, "_login_request_handler")

func _ws_connected(proto = ""):
	print("WS connected")

func _ws_closed(was_clean=false):
	print("WS closed")

func _ws_data():
	print("We have unread messages, polling...")
	request_messages()

func _connect_button_clicked():
	username = $Login/LineEdit.text
	
	do_request("http://127.0.0.1:8080/chat/login"
	, []
	, false
	, HTTPClient.METHOD_POST
	, username)

func _login_request_handler(result, response_code, headers, body):
	var auth_regexp = RegEx.new()
	auth_regexp.compile("auth-example=([^;]+)")
	
	if result == 0 and response_code == 303: # Login ok, See Other
		for s in headers:
			var sid = auth_regexp.search(s)
			# if the connection request succeeded
			if sid:
				# try to connect websocket client
				var err = ws_client.connect_to_url("ws://127.0.0.1:8080/chat/ws/"+username)
				if err == OK:
					
					session_id = sid.get_string(1)
					$Login.hide()
					$Chat.show()
					$HTTPRequest.connect("request_completed", self, "_chat_request_handler")
					request_messages()
					
				return
	
	# If we are here, maybe handle errors

func _chat_request_handler(result, response_code, headers, body):
	print("HTTP RESPONSE RECEIVED " + str(result) + " " + str(response_code))
	if result == 0 and response_code == 200:
		$Chat/RichTextLabel.text += body.get_string_from_utf8()

# handle input event for chat message box
func _chat_text_input(event):
	# If we press the Enter key while writing a message
	# sends the message
	if event is InputEventKey:
		if event.pressed and event.scancode == KEY_ENTER:
			var to_send = $Chat/LineEdit.text
			#$Char/LineEdit.text = ""

			#if to_send == "":
			do_request(
				"http://127.0.0.1:8080/chat/message",
				["Cookie: auth-example=" + session_id],
				false,
				HTTPClient.METHOD_POST,
				to_send
			)

# request the unread messages
func request_messages():
	#print("Requesting messages")
	do_request(
		"http://127.0.0.1:8080/chat/message",
		["Cookie: auth-example=" + session_id],
		false,
		HTTPClient.METHOD_GET,
		""
	)

# called each frame
func _process(delta):
	ws_client.poll()
	request_one()

# HTTP Requests handling
# We need a way to queue HTTP requests as
# a POST can trigger a websocket event which will
# then queue a request to the server.
#
# This happens because we need to GET and POST to
# the server with HTTP, and the HTTPRequest class
# does not handle queued requests.

# queue a HTTP request
func do_request(addr, header, ssl, method, body):
	requests.push_back([addr, header, ssl, method, body])

# called each frame, try to send a request to the HTTP server
func request_one():
	var status = $HTTPRequest.get_http_client_status()
	#print(status)
	if status == HTTPClient.STATUS_DISCONNECTED:
		if len(requests) != 0:
			match requests[0]:
				[var a,var b,var c,var d,var e]:
					$HTTPRequest.request(a,b,c,d,e)
					requests.pop_front()

