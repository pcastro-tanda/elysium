render json: queries.map do |q|
             ^^^^^^^^^^^ `map` is called without a block because the `do` block binds to `render`. Use braces or extract to a variable.
  q.to_h
end
