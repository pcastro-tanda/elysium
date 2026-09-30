render json: queries.map do |q|
  q.to_h
end
