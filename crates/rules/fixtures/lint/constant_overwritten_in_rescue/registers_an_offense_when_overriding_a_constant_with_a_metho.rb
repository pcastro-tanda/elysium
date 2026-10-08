begin
  something
rescue => foo.class::RESCUABLE_EXCEPTIONS
       ^^ `foo.class::RESCUABLE_EXCEPTIONS` is overwritten by `rescue =>`.
end
