extends Node2D

# Her satir bilerek: hangisinin bildirilecegi ve hangisinin sessiz kalacagi
# testte tek tek yaziyor.

func _ready() -> void:
	# Var olanlar.
	$Button.hide()
	$Panel/Deep.hide()
	self.get_node("Button").hide()

	# Yer tutucunun altindaki her sey opak: sahne acilmadan bilinemez.
	$Later/Anything.hide()

	# Calisma aninda kurulan dugum: hicbir .tscn'de yok, ama bir betik ona
	# tam bu adi veriyor.
	var n := Node2D.new()
	n.name = "Runtime"
	add_child(n)
	$Runtime.hide()

	# Baska bir dugume soruluyor: yol O sahneye gore yazilmis.
	var other := Node2D.new()
	other.get_node("Nope").hide()

	# Calisma aninda kurulan yol: gorulemez, yargilanamaz.
	var ad := "Button"
	get_node("Panel/" + ad).hide()
	# %Benzersiz sahibine gore cozuluyor, yola gore degil.
	%Benzersiz.hide()

	# BILDIRILECEK IKI SATIR.
	$Yok.hide()
	get_node("Panel/Gitti").hide()
