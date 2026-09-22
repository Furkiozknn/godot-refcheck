extends Node2D

# Bu betik KOKTE degil, "Kid" dugumunde. `$Sprite` demek "Kid/Sprite" demek;
# koke gore cozulseydi bulunamaz ve uydurma bir bulgu uretirdi.
func _ready() -> void:
	$Sprite.hide()
