extends Node2D


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	AsyncEventBus.message_received.connect(on_message_received)

func on_message_received(message: String) -> void:
	print(message)

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _physics_process(delta: float) -> void:
	if $TextEdit.text == "":
		$TextEdit.text = AsyncEventBus.get_ticket()


func _on_open_button_button_down() -> void:
	AsyncEventBus.open_async_chat()
	pass


func _on_join_button_button_down() -> void:
	AsyncEventBus.join_async_chat($TextEdit.text)


func _on_send_button_button_down() -> void:
	AsyncEventBus.send_message($InputTextEdit.text)
