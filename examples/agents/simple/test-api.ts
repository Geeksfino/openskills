/**
 * Diagnostic script to test DeepSeek API directly
 * This helps determine if the issue is with the API provider or our code
 */

import { createOpenAI } from '@ai-sdk/openai';
import { generateText } from 'ai';
import * as dotenv from 'dotenv';

dotenv.config();

const apiKey = process.env.DEEPSEEK_API_KEY;
if (!apiKey) {
  console.error('DEEPSEEK_API_KEY not set');
  process.exit(1);
}

const openai = createOpenAI({
  apiKey,
  baseURL: 'https://api.deepseek.com/v1',
});

const model = openai('deepseek-chat');

async function testSimpleRequest() {
  console.log('🧪 Test 1: Simple text-only request (no tools)');
  console.log('='.repeat(70));
  try {
    const result = await generateText({
      model,
      prompt: 'Say "Hello, World!"',
      maxSteps: 1,
    });
    console.log('✅ SUCCESS');
    console.log('Response:', result.text);
    return true;
  } catch (error) {
    console.error('❌ FAILED');
    console.error('Error:', error);
    if (error instanceof Error) {
      console.error('Name:', error.name);
      console.error('Message:', error.message);
      console.error('Stack:', error.stack);
    }
    return false;
  }
}

async function testWithTools() {
  console.log('\n🧪 Test 2: Request with tools (but no tool calls)');
  console.log('='.repeat(70));
  try {
    const result = await generateText({
      model,
      prompt: 'Say "Hello, World!"',
      tools: {
        test_tool: {
          description: 'A test tool',
          parameters: {
            type: 'object',
            properties: {
              message: { type: 'string' },
            },
          },
          execute: async ({ message }: { message: string }) => {
            return `Echo: ${message}`;
          },
        },
      },
      maxSteps: 1,
    });
    console.log('✅ SUCCESS');
    console.log('Response:', result.text);
    return true;
  } catch (error) {
    console.error('❌ FAILED');
    console.error('Error:', error);
    if (error instanceof Error) {
      console.error('Name:', error.name);
      console.error('Message:', error.message);
      console.error('Stack:', error.stack);
    }
    return false;
  }
}

async function testLongConversation() {
  console.log('\n🧪 Test 3: Simulated long conversation (multiple steps)');
  console.log('='.repeat(70));
  try {
    const result = await generateText({
      model,
      prompt: 'Count from 1 to 5, saying each number on a new line.',
      maxSteps: 5,
    });
    console.log('✅ SUCCESS');
    console.log('Response:', result.text);
    return true;
  } catch (error) {
    console.error('❌ FAILED');
    console.error('Error:', error);
    if (error instanceof Error) {
      console.error('Name:', error.name);
      console.error('Message:', error.message);
      console.error('Stack:', error.stack);
      
      // Check for specific error details
      const errorObj = error as any;
      if (errorObj.cause) {
        console.error('\nCause:', errorObj.cause);
      }
      if (errorObj.response) {
        console.error('\nResponse status:', errorObj.response.status);
        console.error('Response headers:', errorObj.response.headers);
      }
    }
    return false;
  }
}

async function main() {
  console.log('🔍 DeepSeek API Diagnostic Tests\n');
  
  const results = {
    simple: await testSimpleRequest(),
    withTools: await testWithTools(),
    longConversation: await testLongConversation(),
  };
  
  console.log('\n' + '='.repeat(70));
  console.log('📊 Test Results Summary:');
  console.log('='.repeat(70));
  console.log('Simple request:', results.simple ? '✅ PASS' : '❌ FAIL');
  console.log('With tools:', results.withTools ? '✅ PASS' : '❌ FAIL');
  console.log('Long conversation:', results.longConversation ? '✅ PASS' : '❌ FAIL');
  
  if (!results.simple) {
    console.log('\n⚠️  Even simple requests fail - likely API provider issue');
  } else if (!results.longConversation) {
    console.log('\n⚠️  Long conversations fail - possible timeout/streaming issue');
  } else {
    console.log('\n✅ All tests passed - issue may be specific to agent workflow');
  }
}

main().catch(console.error);
