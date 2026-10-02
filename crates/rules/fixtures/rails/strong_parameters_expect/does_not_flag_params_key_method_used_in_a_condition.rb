if params[:query]&.present?
  filter(params[:query])
end
