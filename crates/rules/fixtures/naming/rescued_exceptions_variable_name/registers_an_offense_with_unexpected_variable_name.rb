begin
  something
rescue ActiveSupport::JSON.my_method => exc
                                        ^^^ Use `e` instead of `exc`.
  # do something
end
