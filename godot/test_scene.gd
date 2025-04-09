extends Node2D


# Called when the node enters the scene tree for the first time.
func _ready() -> void:
	pass # Replace with function body.


# Called every frame. 'delta' is the elapsed time since the previous frame.
func _physics_process(delta: float) -> void:
	pass


func _on_open_button_button_down() -> void:
	AsyncEventBus.open_async_chat()
	$TextEdit.text = AsyncEventBus.get_ticket()
	pass
