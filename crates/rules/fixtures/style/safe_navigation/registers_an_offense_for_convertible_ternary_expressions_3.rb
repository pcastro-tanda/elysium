FOO::BAR ? FOO::BAR.bar : nil
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

FOO::BAR.nil? ? nil : FOO::BAR.bar
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

!FOO::BAR.nil? ? FOO::BAR.bar : nil
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.

!FOO::BAR ? nil : FOO::BAR.bar
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use safe navigation (`&.`) instead of checking if an object exists before calling the method.
