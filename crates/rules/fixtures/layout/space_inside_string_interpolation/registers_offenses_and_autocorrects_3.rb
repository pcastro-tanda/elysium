"#{ var}"
       ^ Use space inside string interpolation.
"#{var }"
 ^^ Use space inside string interpolation.
"#{   var   }"
"#{var	}"
 ^^ Use space inside string interpolation.
"#{	var	}"
"#{	var}"
       ^ Use space inside string interpolation.
"#{ 	 var 	 	}"
