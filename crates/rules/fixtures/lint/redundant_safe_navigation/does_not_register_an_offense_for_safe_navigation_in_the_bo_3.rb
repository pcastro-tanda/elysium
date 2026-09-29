unless foo&.ready?
  foo&.name
else
  bar
end
