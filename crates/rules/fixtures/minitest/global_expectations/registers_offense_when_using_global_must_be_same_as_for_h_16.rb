it 'does something' do
  $n = do_something
  $n[:foo].must_be_same_as 42
  ^^^^^^^^ Use `value($n[:foo])` instead.
end
