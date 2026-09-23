expect { order.expire }.to change { order.events }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Parenthesize the param `change { order.events }` to make sure that the block will be associated with the `change` method call.
