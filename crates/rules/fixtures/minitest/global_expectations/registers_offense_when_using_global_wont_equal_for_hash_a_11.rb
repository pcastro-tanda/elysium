it 'does something' do
  $n = do_something
  $n[:foo].wont_equal 42
  ^^^^^^^^ Use `expect($n[:foo])` instead.
end
