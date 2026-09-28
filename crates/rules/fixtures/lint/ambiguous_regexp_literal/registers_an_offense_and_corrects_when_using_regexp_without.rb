class MyTest
  test '#foo' do
    assert_match /expected/, actual
                 ^ Ambiguous regexp literal. Parenthesize the method arguments if it's surely a regexp literal, or add a whitespace to the right of the `/` if it should be a division.
  end
end
