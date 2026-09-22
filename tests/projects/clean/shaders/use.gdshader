shader_type canvas_item;
#include "base.gdshaderinc"
void fragment() { COLOR.a = half_of(COLOR.a); }
