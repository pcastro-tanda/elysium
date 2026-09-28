var =
  # This is the value of `other:`, like so: `other: condition || other_condition`
  unless object.action value:, other:
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid `unless` branches without a body.
    condition || other_condition
  end
