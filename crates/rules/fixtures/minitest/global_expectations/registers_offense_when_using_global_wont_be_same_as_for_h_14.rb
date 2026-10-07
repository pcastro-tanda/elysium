it 'does something' do
  $n = do_something
  $n[:foo].wont_be_same_as 42
  ^^^^^^^^ Use `expect($n[:foo])` instead.
end
