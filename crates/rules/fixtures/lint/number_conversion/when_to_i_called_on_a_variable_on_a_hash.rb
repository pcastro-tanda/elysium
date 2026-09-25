params[:field].to_i
^^^^^^^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `params[:field].to_i`, use stricter `Integer(params[:field], 10)`.
