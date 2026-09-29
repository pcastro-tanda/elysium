begin
  get something
rescue ActiveResource::Redirection => e
  redirect_to e.response['Location']
end
