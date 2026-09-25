params = { id: 10 }
params[:id].to_i
^^^^^^^^^^^^^^^^ Replace unsafe number conversion with number class parsing, instead of using `params[:id].to_i`, use stricter `Integer(params[:id], 10)`.
