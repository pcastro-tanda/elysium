expect { order.expire }.to not_change { order.events }
