routes.draw do
  match ':controller/:action/:id', via: method
end
