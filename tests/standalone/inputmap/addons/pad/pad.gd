extends Node

# An addon names the actions its own README asks the user to define. Whether
# the user has done that yet is not this project's defect to report.


func _process(_delta: float) -> void:
	if Input.is_action_pressed("pad_confirm"):
		pass
