it 'does something' do
  $n = do_something
  $n[:foo].must_be_within_epsilon 42
  ^^^^^^^^ Use `expect($n[:foo])` instead.
end
