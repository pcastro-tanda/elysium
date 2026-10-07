it 'does something' do
  $n = do_something
  $n[:foo].must_equal 42
  ^^^^^^^^ Use `expect($n[:foo])` instead.
end
