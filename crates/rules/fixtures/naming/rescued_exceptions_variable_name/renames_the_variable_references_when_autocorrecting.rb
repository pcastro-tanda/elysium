begin
  get something
rescue ActiveResource::Redirection => redirection
                                      ^^^^^^^^^^^ Use `e` instead of `redirection`.
  redirect_to redirection.response['Location']
end
