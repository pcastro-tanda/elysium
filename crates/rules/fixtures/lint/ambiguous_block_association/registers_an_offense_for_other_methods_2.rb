expect { order.expire }.to update { order.events }
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Parenthesize the param `update { order.events }` to make sure that the block will be associated with the `update` method call.
