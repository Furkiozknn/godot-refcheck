extends Node

# The one defect in this project: "jmup" is a typo for "jump". The action is
# not in the Input Map, so the check is false on every frame and the jump
# never fires. The engine says so only on the frame this line runs.


func _ready() -> void:
	if Input.is_action_pressed("jmup"):
		print("unreachable")
	InputMap.add_action("dash")


# Every shape below is fine, and none of them may be reported.
func _shapes_that_are_fine(event: InputEvent) -> void:
	# Defined under [input] in project.godot, also as a StringName literal.
	if Input.is_action_just_pressed(&"jump"):
		pass
	var x := Input.get_axis("move_left", "move_right")
	# Built in to every project.
	if event.is_action_pressed("ui_accept"):
		pass
	# Added at run time by _ready() above.
	if Input.is_action_pressed("dash"):
		pass
	# Added through the project settings at run time.
	ProjectSettings.set_setting("input/crouch", {"deadzone": 0.5, "events": []})
	if Input.is_action_pressed("crouch"):
		pass
	# Asked about before it is used.
	if InputMap.has_action("debug_menu") and Input.is_action_just_pressed("debug_menu"):
		pass
	# A computed name cannot be judged.
	var computed := "act_" + str(x)
	if Input.is_action_pressed(computed):
		pass
	# Input.is_action_pressed("commented_out")
	var text := "Input.is_action_pressed(\"in_a_string\")"
	print(text)
