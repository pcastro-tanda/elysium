it 'does something' do
  foo(a).must_be_within_epsilon 0
  ^^^^^^ Use `_(foo(a))` instead.
end
