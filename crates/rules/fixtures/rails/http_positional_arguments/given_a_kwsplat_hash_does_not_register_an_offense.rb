[{ format: :json }, { format: :html }].each do |args|
  get :nothing, **args
end
