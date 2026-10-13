"""Transport regression tests; these do not establish actual host integration."""
import io
import json
import queue
import unittest
from unittest.mock import Mock
from run_codex_host_smoke import Host, require


class TransportTests(unittest.TestCase):
    def host(self, messages):
        host = Host.__new__(Host)
        host.sequence = 0
        host.process = Mock()
        host.process.stdin = io.StringIO()
        host.lines = queue.Queue()
        for message in messages:
            host.lines.put(message)
        return host

    def test_notifications_are_ignored_and_request_is_exact(self):
        host = self.host([json.dumps({'method':'notification','params':{}}),
            json.dumps({'id':99,'result':{}}),json.dumps({'id':1,'result':{'ok':True}})])
        self.assertEqual(host.request('mcpServer/tool/call', {'tool':'regen_document'}), {'ok':True})
        self.assertEqual(json.loads(host.process.stdin.getvalue()),
            {'id':1,'method':'mcpServer/tool/call','params':{'tool':'regen_document'}})

    def test_exit_timeout_and_error_fail_closed(self):
        for messages, timeout in [([''],1), ([],.001),
                ([json.dumps({'id':1,'error':{'message':'private-value'}})],1)]:
            with self.subTest(messages=messages):
                with self.assertRaises(RuntimeError) as error:
                    self.host(messages).request('initialize',{},timeout)
                self.assertNotIn('private-value',str(error.exception))

    def test_refusal_state_cannot_be_confused_with_success(self):
        host = self.host([])
        host.request = Mock(return_value={'isError':True})
        with self.assertRaises(RuntimeError):
            host.tool('owned-thread','apply_sketch_edit',{})
        host.tool('owned-thread','apply_sketch_edit',{},refuse=True)
        with self.assertRaises(RuntimeError):
            require(False,'refused')


if __name__=='__main__':
    unittest.main()
